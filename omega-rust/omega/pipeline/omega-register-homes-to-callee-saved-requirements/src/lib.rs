#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Register homes to validated ABI-preservation requirements.
//!
//! The executable entrance retains exact register-home custody, intersects
//! observed writes with the target-owned ABI preservation roster, and grants
//! no frame layout, save/restore instruction, unwind, or emission authority.

mod callee_saved_requirements;

pub use callee_saved_requirements::*;
