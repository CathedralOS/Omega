//! Provider records that Build, package review and the native compiler share:
//! the receiver's acceptance of a service declaration, the provider plan for one
//! provider type, service schema and target, and the selected plans with their
//! checked installation rows. Provider plans name native targets, so the source
//! compiler must not depend on this crate.

#![forbid(unsafe_code)]

pub mod acceptance;
pub mod plans;
pub mod selections;
