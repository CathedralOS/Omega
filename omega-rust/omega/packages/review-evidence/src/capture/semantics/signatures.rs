//! Public and nested generic-signature projection.

pub(in crate::capture) mod parameters;
pub(crate) mod policy;
pub(in crate::capture) mod policy_crashes;
mod projection;
pub(crate) use projection::SignatureProjection;
