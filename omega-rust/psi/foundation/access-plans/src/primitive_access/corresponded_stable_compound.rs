//! Correspondence-required custody for one already-specialized bounded Stable
//! compound mutation.
//!
//! Generic bounded read-patch-write remains valid for ordinary Stable
//! storage. A consumer that intends to attach physical provider/device meaning
//! must first cross this narrower boundary, which retains the exact non-Clone
//! correspondence by lifetime and independently replays the sealed compound
//! request. This module performs no read, write, provider operation, or target
//! lowering.

#[cfg(test)]
use crate::PlacementPlanId;
use crate::access_plan::diagnostic::{access_plan_rejection, into_validated_access};
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, StableCompoundMutationAccessRequest,
};

/// One exact bounded Stable compound specialization joined to the physical
/// correspondence retained by its originating placed view.
#[derive(Debug)]
#[must_use = "corresponded Stable compound access retains placed and physical provenance"]
pub struct CorrespondedStableCompoundMutationAccessRequest<'view, 'extent> {
    access: StableCompoundMutationAccessRequest<'view, 'extent>,
    correspondence: &'view AdmittedSchemaDeviceCorrespondence,
}

impl<'view, 'extent> CorrespondedStableCompoundMutationAccessRequest<'view, 'extent> {
    /// The exact bounded compound specialization retained by this custody
    /// join.
    pub const fn compound_access(&self) -> &StableCompoundMutationAccessRequest<'view, 'extent> {
        &self.access
    }

    /// The exact lifetime-bound, non-Clone physical correspondence retained
    /// by the originating placed view.
    pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
        self.correspondence
    }

    /// Replay both the complete bounded compound specialization and its exact
    /// correspondence identity before a provider/device consumer proceeds.
    /// Rejection only borrows this carrier, so no read-patch-write occurs and
    /// the complete input remains available for repair and retry.
    pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let replayed = validate_corresponded_stable_compound_access(&self.access)?;
        if !std::ptr::eq(replayed, self.correspondence) {
            return Err(AccessPlanDiagnostic(
                "corresponded Stable compound lowering retained a different schema/device correspondence authority"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Remove only this correspondence-required staging layer. The original
    /// bounded compound specialization and its exclusive authority remain
    /// intact.
    pub fn into_compound_access(self) -> StableCompoundMutationAccessRequest<'view, 'extent> {
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
/// specialized compound request. No provider/device operation is selected or
/// attempted.
    CorrespondedStableCompoundMutationAccessRejection<'view, 'extent> {
        access: StableCompoundMutationAccessRequest<'view, 'extent>,
        diagnostic: AccessPlanDiagnostic,
    }
}

impl<'view, 'extent> StableCompoundMutationAccessRequest<'view, 'extent> {
    /// Require the exact schema/device correspondence retained by this
    /// bounded compound request before handing it to a provider/device-
    /// specific consumer. Ordinary correspondence-free Stable storage rejects
    /// and returns this complete specialization unchanged.
    pub fn into_corresponded_stable_compound_access(
        self,
    ) -> Result<
        CorrespondedStableCompoundMutationAccessRequest<'view, 'extent>,
        CorrespondedStableCompoundMutationAccessRejection<'view, 'extent>,
    > {
        into_validated_access(
            self,
            validate_corresponded_stable_compound_access,
            |access, correspondence| CorrespondedStableCompoundMutationAccessRequest {
                access,
                correspondence,
            },
            |access, diagnostic| CorrespondedStableCompoundMutationAccessRejection {
                access,
                diagnostic,
            },
        )
    }
}

fn validate_corresponded_stable_compound_access<'access, 'view, 'extent>(
    access: &'access StableCompoundMutationAccessRequest<'view, 'extent>,
) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
    access.validate_for_lowering()?;
    access.request._authority.correspondence().ok_or_else(|| {
        AccessPlanDiagnostic(
            "provider/device Stable compound lowering requires admitted schema/device correspondence"
                .into(),
        )
    })
}
