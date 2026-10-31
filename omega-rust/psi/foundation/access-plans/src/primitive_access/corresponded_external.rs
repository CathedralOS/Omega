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
use crate::AccessPlanId;
use crate::{
    AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, ExternalPrimitiveAccessRequest,
};

corresponded_primitive_access!(
    CorrespondedExternalPrimitiveAccessRequest,
    CorrespondedExternalPrimitiveAccessRejection,
    ExternalPrimitiveAccessRequest,
    external_access,
    into_external_access,
    into_corresponded_external_access,
    validate_corresponded_external_access,
    must_use = "corresponded External access retains placed and physical provenance",
    specialization = "External",
    placed_doc = "External specialization and all of its placed authority remain intact.",
    require_doc = "External request before handing it to a provider/device-specific consumer.",
    storage_doc = "External storage rejects and returns this complete specialization unchanged.",
    rejection_event = "transfer",
    required_message =
        "provider/device External lowering requires admitted schema/device correspondence",
    retained_message = "corresponded External lowering retained a different schema/device correspondence authority",
);
