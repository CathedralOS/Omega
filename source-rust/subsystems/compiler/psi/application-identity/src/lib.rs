//! Canonical application and constant identity records and their codecs.
//! No source files, mutable symbol tables or preparation entry points.
#![forbid(unsafe_code)]

pub mod callback_declarations;
pub mod conformance_identity;
pub mod const_value;
pub mod opaque_representations;
pub mod type_identity;

#[cfg(test)]
mod tests;
