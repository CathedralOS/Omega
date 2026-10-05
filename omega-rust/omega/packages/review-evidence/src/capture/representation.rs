//! Review-evidence projection for representation trust boundaries.
//!
//! This entrance coordinates the three representation-TCB sources: unbound
//! opaque declarations, consumer-side physical demands, and producer-side
//! representation availability. Exact semantic-dependency capture and physical
//! boundary-contract translation live in the named leaves below.

pub(super) mod physical_contract;
mod policy;

pub use policy::project_checked_representation_policy;
pub(crate) use policy::rederive_selections;
