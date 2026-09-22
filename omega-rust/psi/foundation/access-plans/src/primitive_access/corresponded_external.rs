//! Correspondence-required custody for one already-specialized External
//! primitive access.
//!
//! Generic External access remains usable for ordinary stable or otherwise
//! non-device storage. A consumer that intends to attach physical device
//! meaning must first cross this narrower boundary, which retains the exact
//! non-Clone correspondence by lifetime and independently replays the sealed
//! External request. This module does not select or perform a device
//! operation, observe storage, or establish target lowering.

#[cfg(test)]
use crate::PlacementPlanId;
use crate::access_plan::diagnostic::{access_plan_rejection, into_validated_access};
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, ExternalPrimitiveAccessRequest,
};

/// One exact External primitive specialization joined to the physical
/// correspondence retained by its originating placed view.
#[derive(Debug)]
#[must_use = "corresponded External access retains placed and physical provenance"]
pub struct CorrespondedExternalPrimitiveAccessRequest<'view, 'extent> {
    access: ExternalPrimitiveAccessRequest<'view, 'extent>,
    correspondence: &'view AdmittedSchemaDeviceCorrespondence,
}

impl<'view, 'extent> CorrespondedExternalPrimitiveAccessRequest<'view, 'extent> {
    /// The exact External specialization retained by this custody join.
    pub const fn external_access(&self) -> &ExternalPrimitiveAccessRequest<'view, 'extent> {
        &self.access
    }

    /// The exact lifetime-bound, non-Clone physical correspondence retained
    /// by the originating placed view.
    pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
        self.correspondence
    }

    /// Replay both the complete External specialization and its exact
    /// correspondence identity before a provider/device consumer proceeds.
    /// Rejection only borrows this carrier, so no transfer occurs and the
    /// complete input remains available for repair and retry.
    pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let replayed = validate_corresponded_external_access(&self.access)?;
        if !std::ptr::eq(replayed, self.correspondence) {
            return Err(AccessPlanDiagnostic(
                "corresponded External lowering retained a different schema/device correspondence authority"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Remove only this correspondence-required staging layer. The original
    /// External specialization and all of its placed authority remain intact.
    pub fn into_external_access(self) -> ExternalPrimitiveAccessRequest<'view, 'extent> {
        self.access
    }

    #[cfg(test)]
    pub(crate) fn replace_request_plan_for_test(
        &mut self,
        placement: PlacementPlanId,
    ) -> PlacementPlanId {
        std::mem::replace(&mut self.access.request.plan, placement)
    }

    #[cfg(test)]
    pub(crate) fn replace_correspondence_for_test(
        &mut self,
        correspondence: &'view AdmittedSchemaDeviceCorrespondence,
    ) -> &'view AdmittedSchemaDeviceCorrespondence {
        std::mem::replace(&mut self.correspondence, correspondence)
    }
}

access_plan_rejection! {
/// Failed correspondence-required staging returns the exact already-
/// specialized External request. No provider/device operation is selected or
/// attempted.
    CorrespondedExternalPrimitiveAccessRejection<'view, 'extent> {
        access: ExternalPrimitiveAccessRequest<'view, 'extent>,
        diagnostic: AccessPlanDiagnostic,
    }
}

impl<'view, 'extent> ExternalPrimitiveAccessRequest<'view, 'extent> {
    /// Require the exact schema/device correspondence retained by this
    /// External request before handing it to a provider/device-specific
    /// consumer. Ordinary correspondence-free External storage rejects and
    /// returns this complete specialization unchanged.
    pub fn into_corresponded_external_access(
        self,
    ) -> Result<
        CorrespondedExternalPrimitiveAccessRequest<'view, 'extent>,
        CorrespondedExternalPrimitiveAccessRejection<'view, 'extent>,
    > {
        into_validated_access(
            self,
            validate_corresponded_external_access,
            |access, correspondence| CorrespondedExternalPrimitiveAccessRequest {
                access,
                correspondence,
            },
            |access, diagnostic| CorrespondedExternalPrimitiveAccessRejection {
                access,
                diagnostic,
            },
        )
    }
}

fn validate_corresponded_external_access<'access, 'view, 'extent>(
    access: &'access ExternalPrimitiveAccessRequest<'view, 'extent>,
) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
    access.validate_for_lowering()?;
    access.request._authority.correspondence().ok_or_else(|| {
        AccessPlanDiagnostic(
            "provider/device External lowering requires admitted schema/device correspondence"
                .into(),
        )
    })
}
