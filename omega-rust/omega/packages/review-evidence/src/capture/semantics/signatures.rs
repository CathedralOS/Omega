//! Public and nested generic-signature projection.

mod callable_scope;
pub(crate) use callable_scope::callable_declaration_scope;
pub(in crate::capture) mod parameters;
pub(crate) mod policy;
pub(in crate::capture) mod policy_crashes;
mod projection;
pub(crate) use projection::SignatureProjection;
