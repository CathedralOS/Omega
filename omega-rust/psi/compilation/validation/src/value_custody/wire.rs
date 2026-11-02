//! Numbered protocol descriptions retain declaration/history validation and
//! invocation-owned compatibility demands. Codec operation selection and byte
//! grammar are authored library code; numbered data grants no synthetic calls.
mod compatibility;
mod schema_validation;
pub use compatibility::validate_wire_protocol;
pub(crate) use schema_validation::validate_wire_schemas;
