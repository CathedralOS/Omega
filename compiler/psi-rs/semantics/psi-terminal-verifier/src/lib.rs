#![forbid(unsafe_code)]

//! Structural validation and proof checking for terminal-Psi modules.
//!
//! The verifier reconstructs semantic axioms from executable operations and
//! edges, then requires evidence for every bodyful contract clause. Proof
//! bundles cannot choose which obligations exist.

mod validation;
mod verification;

pub use validation::*;
pub use verification::*;
