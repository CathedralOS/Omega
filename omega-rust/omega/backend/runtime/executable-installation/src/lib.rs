//! Normalized executable-artifact admission and installation.
//!
//! Start at `executable_installation.rs` for the authority-consuming lifecycle.
//! Container decoding and materialization alone never establish execute authority.

mod executable_installation;

pub use executable_installation::*;
