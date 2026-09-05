#![forbid(unsafe_code)]

//! Unsealed Psi program data between checked lowering and Terminal publication.
//!
//! Begin at lowered_psi.rs. Proof and debug companions are replaceable data;
//! checked-source joins stay explicit sidecars and are excluded from the wire.
//! Neither this record nor its construction grants verification authority.

mod lowered_psi;
pub use lowered_psi::*;
