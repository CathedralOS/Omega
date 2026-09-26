//! Contract propositions: `application` projects one proposition,
//! `binders` its binder arguments and evidence projection, `endpoint` its
//! endpoint and signature, and `evidence` the evidence it carries.

pub(in crate::capture) mod application;
pub(in crate::capture) mod binders;
pub(in crate::capture) mod endpoint;
pub(in crate::capture) mod evidence;
