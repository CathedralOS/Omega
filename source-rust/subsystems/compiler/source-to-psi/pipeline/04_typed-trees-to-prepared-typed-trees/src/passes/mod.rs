//! The passes of typed preparation, one module each. Their order is the list
//! in `src/lib.rs`.

pub(super) mod conformance_headers;
pub(crate) mod domain_applications;
pub(crate) mod specialization;
