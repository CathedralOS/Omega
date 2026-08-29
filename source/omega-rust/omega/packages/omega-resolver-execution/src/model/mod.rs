//! Closed resolver phases, guarantees, and observed execution policy.

mod guarantees;
mod phase;
mod policy;

pub use guarantees::{
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionGuaranteeRow, ResolverStrictExecutionUnavailable,
};
pub use phase::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
pub use policy::{
    ResolverExecutionBackendIdentity, ResolverExecutionPolicyObservation,
    ResolverExecutionResourceCeilings,
};
