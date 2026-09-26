//! Contract propositions: `application` projects one proposition,
//! `binders` its binder arguments and evidence projection, `endpoint` its
//! endpoint and signature, and `evidence` the evidence it carries.

pub(in crate::package_evidence::capture) mod application;
pub(in crate::package_evidence::capture) mod binders;
pub(in crate::package_evidence::capture) mod endpoint;
pub(in crate::package_evidence::capture) mod evidence;
