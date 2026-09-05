#![forbid(unsafe_code)]

//! Physical instruction programs after register assignment and before encoding.
//!
//! Start at [`physical_instructions::PostAllocationMachinePlan`]. This crate
//! owns data and canonical encoding, not construction or admission authority.

pub mod physical_instructions;
pub use physical_instructions::*;
