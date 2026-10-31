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
use crate::AccessPlanId;
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, StablePrimitiveAccessRequest,
};

corresponded_primitive_access!(
    CorrespondedStablePrimitiveAccessRequest,
    CorrespondedStablePrimitiveAccessRejection,
    StablePrimitiveAccessRequest,
    stable_access,
    into_stable_access,
    into_corresponded_stable_access,
    validate_corresponded_stable_access,
    must_use = "corresponded Stable access retains placed and physical provenance",
    specialization = "Stable",
    placed_doc = "Stable specialization and all of its placed authority remain intact.",
    require_doc = "Stable request before handing it to a provider/device-specific consumer.",
    storage_doc = "Stable storage rejects and returns this complete specialization unchanged.",
    rejection_event = "memory event",
    required_message =
        "provider/device Stable lowering requires admitted schema/device correspondence",
    retained_message =
        "corresponded Stable lowering retained a different schema/device correspondence authority",
);
