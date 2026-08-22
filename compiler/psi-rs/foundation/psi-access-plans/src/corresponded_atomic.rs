//! Correspondence-required custody for one already-specialized Atomic
//! primitive access.
//!
//! Generic Atomic access remains valid for ordinary atomic storage. A
//! consumer that intends to attach physical device meaning must first cross
//! this narrower boundary, which retains the exact non-Clone correspondence
//! by lifetime and independently replays the sealed Atomic request. This
//! module does not select or attempt an atomic operation, mutate storage, or
//! establish target lowering.

#[cfg(test)]
use super::PlacementPlanId;
use super::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, AtomicPrimitiveAccessRequest,
};

/// One exact Atomic primitive specialization joined to the physical
/// correspondence retained by its originating placed view.
#[derive(Debug)]
#[must_use = "corresponded Atomic access retains placed and physical provenance"]
pub struct CorrespondedAtomicPrimitiveAccessRequest<'view, 'extent> {
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
    correspondence: &'view AdmittedSchemaDeviceCorrespondence,
}

impl<'view, 'extent> CorrespondedAtomicPrimitiveAccessRequest<'view, 'extent> {
    /// The exact Atomic specialization retained by this custody join.
    pub const fn atomic_access(&self) -> &AtomicPrimitiveAccessRequest<'view, 'extent> {
        &self.access
    }

    /// The exact lifetime-bound, non-Clone physical correspondence retained
    /// by the originating placed view.
    pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
        self.correspondence
    }

    /// Replay both the complete Atomic specialization and its exact
    /// correspondence identity before a provider/device consumer proceeds.
    /// Rejection only borrows this carrier, so no atomic attempt occurs and
    /// the complete input remains available for repair and retry.
    pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let replayed = validate_corresponded_atomic_access(&self.access)?;
        if !std::ptr::eq(replayed, self.correspondence) {
            return Err(AccessPlanDiagnostic(
                "corresponded Atomic lowering retained a different schema/device correspondence authority"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Remove only this correspondence-required staging layer. The original
    /// Atomic specialization and all of its placed authority remain intact.
    pub fn into_atomic_access(self) -> AtomicPrimitiveAccessRequest<'view, 'extent> {
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
/// specialized Atomic request. No provider/device operation is selected or
/// attempted.
#[derive(Debug)]
pub struct CorrespondedAtomicPrimitiveAccessRejection<'view, 'extent> {
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> CorrespondedAtomicPrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        AtomicPrimitiveAccessRequest<'view, 'extent>,
        AccessPlanDiagnostic,
    ) {
        (self.access, self.diagnostic)
    }
}

impl<'view, 'extent> AtomicPrimitiveAccessRequest<'view, 'extent> {
    /// Require the exact schema/device correspondence retained by this Atomic
    /// request before handing it to a provider/device-specific consumer.
    /// Ordinary correspondence-free atomic storage rejects and returns this
    /// complete specialization unchanged.
    pub fn into_corresponded_atomic_access(
        self,
    ) -> Result<
        CorrespondedAtomicPrimitiveAccessRequest<'view, 'extent>,
        CorrespondedAtomicPrimitiveAccessRejection<'view, 'extent>,
    > {
        let correspondence = match validate_corresponded_atomic_access(&self) {
            Ok(correspondence) => correspondence,
            Err(diagnostic) => {
                return Err(CorrespondedAtomicPrimitiveAccessRejection {
                    access: self,
                    diagnostic,
                });
            }
        };
        Ok(CorrespondedAtomicPrimitiveAccessRequest {
            access: self,
            correspondence,
        })
    }
}

fn validate_corresponded_atomic_access<'access, 'view, 'extent>(
    access: &'access AtomicPrimitiveAccessRequest<'view, 'extent>,
) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
    access.validate_for_lowering()?;
    access.request._authority.correspondence().ok_or_else(|| {
        AccessPlanDiagnostic(
            "provider/device Atomic lowering requires admitted schema/device correspondence".into(),
        )
    })
}
