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
use crate::AccessPlanId;
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, StableCompoundMutationAccessRequest,
};

corresponded_primitive_access!(
    CorrespondedStableCompoundMutationAccessRequest,
    CorrespondedStableCompoundMutationAccessRejection,
    StableCompoundMutationAccessRequest,
    compound_access,
    into_compound_access,
    into_corresponded_stable_compound_access,
    validate_corresponded_stable_compound_access,
    must_use = "corresponded Stable compound access retains placed and physical provenance",
    specialization = "bounded Stable compound",
    placed_doc = "bounded compound specialization and its exclusive authority remain intact.",
    require_doc =
        "bounded compound request before handing it to a provider/device-specific consumer.",
    storage_doc = "Stable storage rejects and returns this complete specialization unchanged.",
    rejection_event = "read-patch-write",
    required_message =
        "provider/device Stable compound lowering requires admitted schema/device correspondence",
    retained_message = "corresponded Stable compound lowering retained a different schema/device correspondence authority",
);
