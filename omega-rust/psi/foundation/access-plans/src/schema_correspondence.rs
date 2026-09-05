//! Provider-issued correspondence between one nominal placement and one
//! stable physical device instance.
//!
//! Resource-profile compatibility answers whether a range can support a
//! placement. This module deliberately carries the separate admitted claim
//! that the placement describes the named device. It performs no device read,
//! placement admission, content establishment, or field access.

use super::{
    AccessFieldKey, AccessPlanDiagnostic, AdmittedResourceProfile, BorrowPolarity,
    PlacedFieldProjection, PlacedView, PlacementAdmission, PlacementAdmissionId,
    PlacementAuthorityRef, PlacementPlanId, ResourceProfileReceiptId, ValidatedPlacementPlan,
    place, project_placed_field, validate_placement_admission,
};
use extents::{ExtentLoan, LoanPolarity};

macro_rules! normalized_identity {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
                if identity == 0 {
                    return Err(AccessPlanDiagnostic(
                        concat!($label, " cannot be zero").into(),
                    ));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

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
/// This remains separate from `PlacementResourceCompatibility`: compatibility
/// cannot manufacture physical meaning. Optional revision evidence must name
/// the same provider, device, and resource-profile grant before this carrier
/// can be formed.
#[derive(Debug)]
#[must_use = "schema correspondence retains admitted provider/device provenance"]
pub struct SchemaDeviceCorrespondenceGrant {
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    source: SchemaCorrespondenceSourceId,
    placement: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    revision: Option<RuntimeDeviceRevisionEvidence>,
}

impl SchemaDeviceCorrespondenceGrant {
    pub fn from_admitted_provider(
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        source: SchemaCorrespondenceSourceId,
        placement: &ValidatedPlacementPlan,
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
        placement: &ValidatedPlacementPlan,
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
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

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
    placement: ValidatedPlacementPlan,
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
    placement: ValidatedPlacementPlan,
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

    pub const fn placement(&self) -> PlacementPlanId {
        self.placement.identity()
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
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

    fn validate_structure(&self) -> Result<(), AccessPlanDiagnostic> {
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
    pub(super) fn replace_placement_for_test(
        &mut self,
        placement: PlacementPlanId,
    ) -> PlacementPlanId {
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
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (SchemaDeviceCorrespondenceGrant, AccessPlanDiagnostic) {
        (self.grant, self.diagnostic)
    }
}

fn admit_schema_device_correspondence(
    grant: SchemaDeviceCorrespondenceGrant,
    placement: &ValidatedPlacementPlan,
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

/// One borrowed placement admission joined to its separate admitted physical
/// correspondence. The fields remain distinct: correspondence contributes no
/// storage compatibility, loan, content, or access authority.
#[derive(Debug)]
#[must_use = "corresponded placement admission retains loan and physical provenance"]
pub struct SchemaCorrespondedPlacementAdmission<'extent> {
    admission: PlacementAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
}

impl<'extent> SchemaCorrespondedPlacementAdmission<'extent> {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission.identity
    }

    pub const fn correspondence(&self) -> &AdmittedSchemaDeviceCorrespondence {
        &self.correspondence
    }

    pub fn into_parts(
        self,
    ) -> (
        PlacementAdmission<'extent>,
        AdmittedSchemaDeviceCorrespondence,
    ) {
        (self.admission, self.correspondence)
    }

    /// Cancel the permission-only placement while preserving the separately
    /// admitted correspondence for a later matching placement.
    pub fn withdraw(self) -> (ExtentLoan<'extent>, AdmittedSchemaDeviceCorrespondence) {
        (self.admission.withdraw(), self.correspondence)
    }

    /// Establish the borrowed view only after independently replaying both the
    /// correspondence relation and the retained placement admission. The
    /// resulting carrier retains the exact correspondence through every
    /// projection derived from it.
    pub fn establish_view(
        self,
    ) -> Result<
        SchemaCorrespondedPlacedView<'extent>,
        SchemaCorrespondedPlaceEstablishmentError<'extent>,
    > {
        if let Err(diagnostic) =
            validate_schema_correspondence_placement_binding(&self.admission, &self.correspondence)
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
            Ok(view) => Ok(SchemaCorrespondedPlacedView {
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
    pub(super) fn replace_correspondence_placement_for_test(
        &mut self,
        placement: PlacementPlanId,
    ) -> PlacementPlanId {
        self.correspondence.replace_placement_for_test(placement)
    }
}

/// Established borrowed view retaining its separate physical correspondence.
///
#[derive(Debug)]
#[must_use = "corresponded placed view retains loan and physical provenance"]
pub struct SchemaCorrespondedPlacedView<'extent> {
    view: PlacedView<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
}

impl<'extent> SchemaCorrespondedPlacedView<'extent> {
    pub const fn admission(&self) -> PlacementAdmissionId {
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

    /// End this corresponded placed view after independently replaying both
    /// its exact borrowed placement authority and physical correspondence.
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

    /// Project one field while retaining this exact admitted correspondence
    /// as part of the placement authority borrowed by the projection.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Exclusively project one field while retaining this exact admitted
    /// correspondence as part of the placement authority.
    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        let source_loan = match self.view.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        project_placed_field(
            &self.view.plan,
            self.view.profile_receipt,
            &self.view.resources,
            self.view.admission,
            self.view.loan.base(),
            key,
            current_borrow,
            source_loan,
            None,
            PlacementAuthorityRef::CorrespondedBorrowed(self),
        )
    }

    pub(super) const fn view(&self) -> &PlacedView<'extent> {
        &self.view
    }

    pub(super) fn validate_correspondence(&self) -> Result<(), AccessPlanDiagnostic> {
        self.correspondence.validate_structure()?;
        if self.correspondence.placement != self.view.plan
            || self.correspondence.profile_receipt != self.view.profile_receipt
        {
            return Err(AccessPlanDiagnostic(
                "corresponded placed view could not replay its exact placement and resource-profile correspondence"
                    .into(),
            ));
        }
        Ok(())
    }

    fn validate_retirement(&self) -> Result<(), AccessPlanDiagnostic> {
        self.validate_correspondence()?;
        self.view
            .validate_authority("corresponded placed-view retirement")
    }

    #[cfg(test)]
    pub(super) fn replace_correspondence_placement_for_test(
        &mut self,
        placement: PlacementPlanId,
    ) -> PlacementPlanId {
        self.correspondence.replace_placement_for_test(placement)
    }

    #[cfg(test)]
    pub(super) fn replace_view_profile_receipt_for_test(
        &mut self,
        receipt: ResourceProfileReceiptId,
    ) -> ResourceProfileReceiptId {
        std::mem::replace(&mut self.view.profile_receipt, receipt)
    }

    #[cfg(test)]
    pub(super) fn replace_correspondence_profile_receipt_for_test(
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
    view: SchemaCorrespondedPlacedView<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondedPlaceRetirementError<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (SchemaCorrespondedPlacedView<'extent>, AccessPlanDiagnostic) {
        (self.view, self.diagnostic)
    }
}

/// Failed corresponded-view establishment returns the complete bound carrier;
/// its loan and physical provenance remain available for repair or withdrawal.
#[derive(Debug)]
pub struct SchemaCorrespondedPlaceEstablishmentError<'extent> {
    bound: SchemaCorrespondedPlacementAdmission<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondedPlaceEstablishmentError<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SchemaCorrespondedPlacementAdmission<'extent>,
        AccessPlanDiagnostic,
    ) {
        (self.bound, self.diagnostic)
    }
}

/// Failed placement/correspondence binding returns both exact non-Clone
/// inputs. No loan is released and no physical meaning is attached to storage.
#[derive(Debug)]
pub struct SchemaCorrespondencePlacementBindingError<'extent> {
    admission: PlacementAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> SchemaCorrespondencePlacementBindingError<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PlacementAdmission<'extent>,
        AdmittedSchemaDeviceCorrespondence,
        AccessPlanDiagnostic,
    ) {
        (self.admission, self.correspondence, self.diagnostic)
    }
}

/// Join one correspondence fact to the exact borrowed placement admission it
/// describes. This is a custody relation only; it establishes no placed view,
/// content qualification, field access, or device operation.
pub fn bind_schema_correspondence_to_placement<'extent>(
    admission: PlacementAdmission<'extent>,
    correspondence: AdmittedSchemaDeviceCorrespondence,
) -> Result<
    SchemaCorrespondedPlacementAdmission<'extent>,
    SchemaCorrespondencePlacementBindingError<'extent>,
> {
    let diagnostic = validate_schema_correspondence_placement_binding(&admission, &correspondence);
    if let Err(diagnostic) = diagnostic {
        return Err(SchemaCorrespondencePlacementBindingError {
            admission,
            correspondence,
            diagnostic,
        });
    }
    Ok(SchemaCorrespondedPlacementAdmission {
        admission,
        correspondence,
    })
}

fn validate_schema_correspondence_placement_binding(
    admission: &PlacementAdmission<'_>,
    correspondence: &AdmittedSchemaDeviceCorrespondence,
) -> Result<(), AccessPlanDiagnostic> {
    correspondence.validate_structure()?;
    if correspondence.placement != admission.placement_plan
        || correspondence.profile_receipt != admission.profile_receipt
        || admission.profile.receipt() != admission.profile_receipt
    {
        return Err(AccessPlanDiagnostic(
            "schema correspondence does not bind the placement admission's exact plan and resource-profile receipt"
                .into(),
        ));
    }
    let replayed = validate_placement_admission(
        &admission.loan,
        &admission.placement_plan,
        &admission.profile,
    )
    .map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "schema correspondence could not replay the retained placement admission: {diagnostic}"
        ))
    })?;
    if replayed != admission.resources {
        return Err(AccessPlanDiagnostic(
            "schema correspondence replayed resource compatibility differs from the retained placement admission"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessPlan, BoundaryReach, PlacementPlan, validate_placement_plan};
    use layout_plans::LayoutPlanReport;

    fn provider_id(identity: u64) -> SchemaCorrespondenceProviderId {
        SchemaCorrespondenceProviderId::from_normalized_identity(identity).expect("provider")
    }

    fn device_id(identity: u64) -> StableDeviceInstanceId {
        StableDeviceInstanceId::from_normalized_identity(identity).expect("device")
    }

    fn source(identity: u64) -> SchemaCorrespondenceSourceId {
        SchemaCorrespondenceSourceId::from_normalized_identity(identity).expect("source")
    }

    fn receipt_id(identity: u64) -> ResourceProfileReceiptId {
        ResourceProfileReceiptId::from_normalized_identity(identity).expect("profile receipt")
    }

    fn test_placement(schema_report_fingerprint: u64) -> ValidatedPlacementPlan {
        let layout = LayoutPlanReport {
            schema_report_fingerprint,
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: Some(0),
            align: 1,
        };
        let access = AccessPlan::inaccessible(&layout).expect("empty inaccessible access plan");
        validate_placement_plan(PlacementPlan {
            layout,
            access,
            reach: BoundaryReach::default(),
        })
        .expect("empty validated placement")
    }

    fn revision(
        provider: SchemaCorrespondenceProviderId,
        device: StableDeviceInstanceId,
        receipt: ResourceProfileReceiptId,
    ) -> RuntimeDeviceRevisionEvidence {
        RuntimeDeviceRevisionEvidence::from_admitted_provider(
            RuntimeDeviceRevisionObservationId::from_normalized_identity(41).expect("observation"),
            DeviceRevisionPredicateId::from_normalized_identity(42).expect("predicate"),
            provider,
            device,
            receipt,
            0x17,
        )
    }

    #[test]
    fn correspondence_admission_retains_separate_provider_device_and_revision_evidence() {
        let provider = provider_id(7);
        let device = device_id(8);
        let receipt = receipt_id(9);
        let placement = test_placement(11);
        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            source(10),
            &placement,
            receipt,
            Some(revision(provider, device, receipt)),
        )
        .expect("provider correspondence grant");

        let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
            .expect("exact placement/profile join");
        assert_eq!(admitted.provider(), provider);
        assert_eq!(admitted.device(), device);
        assert_eq!(admitted.source().normalized_identity(), 10);
        assert_eq!(admitted.placement_plan(), &placement);
        assert_eq!(admitted.profile_receipt(), receipt);
        let revision = admitted.revision().expect("revision evidence");
        assert_eq!(revision.observed_revision(), 0x17);
        assert_eq!(revision.provider(), provider);
        assert_eq!(revision.device(), device);
        assert_eq!(revision.profile_receipt(), receipt);
    }

    #[test]
    fn revision_and_admission_drift_return_exact_grants_for_retry() {
        let provider = provider_id(17);
        let device = device_id(18);
        let receipt = receipt_id(19);
        let placement = test_placement(21);
        let revision = revision(provider_id(99), device, receipt);
        let revision_observation = revision.observation();
        let rejection = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            source(20),
            &placement,
            receipt,
            Some(revision),
        )
        .expect_err("foreign-provider revision evidence must reject");
        assert!(rejection.diagnostic().0.contains("stable device instance"));
        let (revision, _) = rejection.into_parts();
        assert_eq!(
            revision.expect("returned revision evidence").observation(),
            revision_observation
        );

        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            source(20),
            &placement,
            receipt,
            None,
        )
        .expect("unconditional provider correspondence grant");
        let drifted_placement = test_placement(22);
        let rejection = admit_schema_device_correspondence(grant, &drifted_placement, receipt)
            .expect_err("placement drift must reject");
        assert!(rejection.diagnostic().0.contains("validated placement"));
        let (grant, _) = rejection.into_parts();

        let rejection = admit_schema_device_correspondence(grant, &placement, receipt_id(23))
            .expect_err("profile-grant drift must reject");
        assert!(rejection.diagnostic().0.contains("resource-profile grant"));
        let (grant, _) = rejection.into_parts();

        let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
            .expect("returned provider grant supports corrected retry");
        assert_eq!(admitted.provider(), provider);
        assert_eq!(admitted.device(), device);
        assert!(admitted.revision().is_none());
    }

    #[test]
    fn admission_replays_revision_binding_and_returns_grant_for_retry() {
        let provider = provider_id(27);
        let device = device_id(28);
        let receipt = receipt_id(29);
        let placement = test_placement(31);
        let mut grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            source(30),
            &placement,
            receipt,
            Some(revision(provider, device, receipt)),
        )
        .expect("provider correspondence grant");
        grant.revision.as_mut().expect("revision evidence").device = device_id(99);

        let rejection = admit_schema_device_correspondence(grant, &placement, receipt)
            .expect_err("admission must independently replay revision binding");
        assert!(rejection.diagnostic().0.contains("could not replay"));
        let (mut grant, _) = rejection.into_parts();
        grant
            .revision
            .as_mut()
            .expect("returned revision evidence")
            .device = device;

        let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
            .expect("repaired grant remains valid for retry");
        assert_eq!(admitted.device(), device);
        assert_eq!(
            admitted.revision().expect("revision evidence").device(),
            device
        );
    }

    #[test]
    fn receipt_context_compares_complete_correspondence_structure() {
        let provider = provider_id(37);
        let device = device_id(38);
        let receipt = receipt_id(39);
        let placement = test_placement(40);
        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            source(41),
            &placement,
            receipt,
            Some(revision(provider, device, receipt)),
        )
        .expect("provider correspondence grant");
        let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
            .expect("exact placement/profile join");
        let context = admitted.receipt_context();

        let assert_drift = |drifted: &SchemaDeviceCorrespondenceReceiptContext| {
            assert_eq!(context.device(), drifted.device());
            assert_eq!(context.placement.identity(), drifted.placement.identity());
            assert_ne!(&context, drifted);
        };

        assert_eq!(context, context.clone());
        assert_eq!(context.provider(), provider);
        assert_eq!(context.device(), device);

        let mut provider_drift = context.clone();
        provider_drift.provider = provider_id(42);
        assert_drift(&provider_drift);

        let mut source_drift = context.clone();
        source_drift.source = source(43);
        assert_drift(&source_drift);

        let mut profile_drift = context.clone();
        profile_drift.profile_receipt = receipt_id(44);
        assert_drift(&profile_drift);

        let mut revision_absent = context.clone();
        revision_absent.revision = None;
        assert_drift(&revision_absent);

        let mut revision_observation_drift = context.clone();
        revision_observation_drift
            .revision
            .as_mut()
            .expect("revision context")
            .observation =
            RuntimeDeviceRevisionObservationId::from_normalized_identity(46).expect("observation");
        assert_drift(&revision_observation_drift);

        let mut revision_predicate_drift = context.clone();
        revision_predicate_drift
            .revision
            .as_mut()
            .expect("revision context")
            .predicate =
            DeviceRevisionPredicateId::from_normalized_identity(47).expect("predicate");
        assert_drift(&revision_predicate_drift);

        let mut revision_provider_drift = context.clone();
        revision_provider_drift
            .revision
            .as_mut()
            .expect("revision context")
            .provider = provider_id(48);
        assert_drift(&revision_provider_drift);

        let mut revision_device_drift = context.clone();
        revision_device_drift
            .revision
            .as_mut()
            .expect("revision context")
            .device = device_id(49);
        assert_drift(&revision_device_drift);

        let mut revision_profile_drift = context.clone();
        revision_profile_drift
            .revision
            .as_mut()
            .expect("revision context")
            .profile_receipt = receipt_id(50);
        assert_drift(&revision_profile_drift);

        let mut revision_drift = context.clone();
        revision_drift
            .revision
            .as_mut()
            .expect("revision context")
            .observed_revision = 0x18;
        assert_drift(&revision_drift);

        let mut placement_drift = context.clone();
        placement_drift.placement.layout.schema_report_fingerprint = 45;
        assert_drift(&placement_drift);
    }
}
