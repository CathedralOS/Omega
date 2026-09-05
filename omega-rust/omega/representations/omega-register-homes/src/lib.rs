#![forbid(unsafe_code)]

//! Physical register homes as data, independent of the allocating algorithm.
//!
//! Start at [`register_homes`]. Decode and identity checks establish canonical
//! representation only; allocation validity belongs to the independent verifier.

pub mod register_homes;
pub use register_homes::*;
