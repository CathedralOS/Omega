//! Closed resolver phases, guarantees, and observed execution policy.

mod guarantees;
mod phase;
mod policy;

pub use guarantees::{
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionGuaranteeRow,
};
pub use phase::ResolverExecutionPhase;
pub use policy::{
    ResolverExecutionBackendIdentity, ResolverExecutionPolicyObservation,
    ResolverExecutionResourceCeilings,
};
