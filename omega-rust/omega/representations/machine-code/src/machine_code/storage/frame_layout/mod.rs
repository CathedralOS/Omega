//! Immutable frame geometry and the requirements from which it was derived.
//!
//! These records grant no validation or emission authority. The backend owns
//! construction, independent replay, and the subsequent frame protocol.

mod callee_save;
mod layout;
mod spill;

pub use callee_save::*;
pub use layout::*;
pub use spill::*;
