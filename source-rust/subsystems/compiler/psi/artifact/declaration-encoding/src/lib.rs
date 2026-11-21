//! Canonical encodings of Psi declarations used by the artifact format and native program identities.
#![forbid(unsafe_code)]

pub mod access;
pub mod content_projections;
pub mod crash_routes;
pub mod dispatch;
pub mod domains;
pub mod error;
pub mod fields;
mod integer_types;
pub mod invocations;
pub mod places;
pub mod quotients;
pub mod results;
pub mod structural_fields;
pub mod terms;
pub mod wire;

pub use access::lifecycles::encode_access_lifecycle_catalog;
pub use domains::encode_structural_domain_declaration;
pub use error::CodecError;
use error::malformed;
pub use integer_types::EncodedIntegerTypes;
pub use quotients::encode_quotient_catalog;
