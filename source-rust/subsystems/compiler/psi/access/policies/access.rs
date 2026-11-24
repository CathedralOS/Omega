//! Access decisions, operations, reach and offered resource capabilities.
//! These descriptive values cannot admit a placement or authorize live access.
#![forbid(unsafe_code)]
mod access_operations;
mod boundary_reach;
mod diagnostic;
mod fields;
mod normalized_identity;
mod observation;
mod resources;
pub use access_operations::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
pub use boundary_reach::{BoundaryReach, BoundaryServiceReachId};
pub use diagnostic::AccessPlanDiagnostic;
pub use fields::{
    AccessExposure, AccessPermissions, AtomicPermissions, ExternalRead, FieldAccess,
    ObservationModel,
};
pub(crate) use normalized_identity::normalized_identity;
pub use observation::ObservationDemand;
pub use resources::{
    AtomicCapability, AtomicTransferRule, ExternalCapability, ExternalReadBehavior,
    PeerWritability, ResourceProfile, ResourceRegion, StableCapability, TransferRule,
};
normalized_identity!(
    ResourceProfileReceiptId,
    "resource-profile receipt identity cannot be zero"
);
