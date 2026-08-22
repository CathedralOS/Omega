//! Correspondence-required custody for one already-specialized Stable
//! primitive access.
//!
//! Generic Stable access remains valid for ordinary storage. A consumer that
//! intends to attach physical provider/device meaning must first cross this
//! narrower boundary, which retains the exact non-Clone correspondence by
//! lifetime and independently replays the sealed Stable request. This module
//! does not select or perform an operation, observe or mutate storage, or
//! establish target lowering.

#[cfg(test)]
use super::PlacementPlanId;
use super::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, StablePrimitiveAccessRequest,
};

/// One exact Stable primitive specialization joined to the physical
/// correspondence retained by its originating placed view.
#[derive(Debug)]
#[must_use = "corresponded Stable access retains placed and physical provenance"]
pub struct CorrespondedStablePrimitiveAccessRequest<'view, 'extent> {
    access: StablePrimitiveAccessRequest<'view, 'extent>,
    correspondence: &'view AdmittedSchemaDeviceCorrespondence,
}

impl<'view, 'extent> CorrespondedStablePrimitiveAccessRequest<'view, 'extent> {
    /// The exact Stable specialization retained by this custody join.
    pub const fn stable_access(&self) -> &StablePrimitiveAccessRequest<'view, 'extent> {
        &self.access
    }

    /// The exact lifetime-bound, non-Clone physical correspondence retained
    /// by the originating placed view.
    pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
        self.correspondence
    }

    /// Replay both the complete Stable specialization and its exact
    /// correspondence identity before a provider/device consumer proceeds.
    /// Rejection only borrows this carrier, so no memory event occurs and the
    /// complete input remains available for repair and retry.
    pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let replayed = validate_corresponded_stable_access(&self.access)?;
        if !std::ptr::eq(replayed, self.correspondence) {
            return Err(AccessPlanDiagnostic(
                "corresponded Stable lowering retained a different schema/device correspondence authority"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Remove only this correspondence-required staging layer. The original
    /// Stable specialization and all of its placed authority remain intact.
    pub fn into_stable_access(self) -> StablePrimitiveAccessRequest<'view, 'extent> {
        self.access
    }

    #[cfg(test)]
    pub(super) fn replace_request_plan_for_test(
        &mut self,
        placement: PlacementPlanId,
    ) -> PlacementPlanId {
        std::mem::replace(&mut self.access.request.plan, placement)
    }

    #[cfg(test)]
    pub(super) fn replace_correspondence_for_test(
        &mut self,
        correspondence: &'view AdmittedSchemaDeviceCorrespondence,
    ) -> &'view AdmittedSchemaDeviceCorrespondence {
        std::mem::replace(&mut self.correspondence, correspondence)
    }
}

/// Failed correspondence-required staging returns the exact already-
/// specialized Stable request. No provider/device operation is selected or
/// attempted.
#[derive(Debug)]
pub struct CorrespondedStablePrimitiveAccessRejection<'view, 'extent> {
    access: StablePrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> CorrespondedStablePrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        StablePrimitiveAccessRequest<'view, 'extent>,
        AccessPlanDiagnostic,
    ) {
        (self.access, self.diagnostic)
    }
}

impl<'view, 'extent> StablePrimitiveAccessRequest<'view, 'extent> {
    /// Require the exact schema/device correspondence retained by this Stable
    /// request before handing it to a provider/device-specific consumer.
    /// Ordinary correspondence-free Stable storage rejects and returns this
    /// complete specialization unchanged.
    pub fn into_corresponded_stable_access(
        self,
    ) -> Result<
        CorrespondedStablePrimitiveAccessRequest<'view, 'extent>,
        CorrespondedStablePrimitiveAccessRejection<'view, 'extent>,
    > {
        let correspondence = match validate_corresponded_stable_access(&self) {
            Ok(correspondence) => correspondence,
            Err(diagnostic) => {
                return Err(CorrespondedStablePrimitiveAccessRejection {
                    access: self,
                    diagnostic,
                });
            }
        };
        Ok(CorrespondedStablePrimitiveAccessRequest {
            access: self,
            correspondence,
        })
    }
}

fn validate_corresponded_stable_access<'access, 'view, 'extent>(
    access: &'access StablePrimitiveAccessRequest<'view, 'extent>,
) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
    access.validate_for_lowering()?;
    access.request._authority.correspondence().ok_or_else(|| {
        AccessPlanDiagnostic(
            "provider/device Stable lowering requires admitted schema/device correspondence".into(),
        )
    })
}
