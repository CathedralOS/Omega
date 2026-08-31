#![forbid(unsafe_code)]

//! Structural validation and proof checking for terminal-Psi modules.
//!
//! The verifier reconstructs semantic axioms from executable operations and
//! edges, then requires evidence for every bodyful contract clause. Proof
//! bundles cannot choose which obligations exist.

mod quotient_correspondence;
mod terminal_trace_v1;
mod validation;
mod verification;

pub use quotient_correspondence::*;
pub use terminal_trace_v1::*;
pub use validation::*;
pub use verification::*;
