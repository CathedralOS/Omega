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
use crate::AccessPlanId;
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, AtomicPrimitiveAccessRequest,
};

corresponded_primitive_access!(
    CorrespondedAtomicPrimitiveAccessRequest,
    CorrespondedAtomicPrimitiveAccessRejection,
    AtomicPrimitiveAccessRequest,
    atomic_access,
    into_atomic_access,
    into_corresponded_atomic_access,
    validate_corresponded_atomic_access,
    must_use = "corresponded Atomic access retains placed and physical provenance",
    specialization = "Atomic",
    placed_doc = "Atomic specialization and all of its placed authority remain intact.",
    require_doc = "Atomic request before handing it to a provider/device-specific consumer.",
    storage_doc = "atomic storage rejects and returns this complete specialization unchanged.",
    rejection_event = "atomic attempt",
    required_message =
        "provider/device Atomic lowering requires admitted schema/device correspondence",
    retained_message =
        "corresponded Atomic lowering retained a different schema/device correspondence authority",
);
