//! Optimizer module role: stage group. Deterministic fixed-point execution across exact Psi passes.
//!
//! Single-pass convergence is grouped by the behavior being stabilized: algebraic
//! rewrites, structural rewrites, proof-check elision, and value numbering. The
//! dispatch-and-composition leaf owns behavior spanning pass-manager boundaries.

mod algebraic_rewrites;
mod dead_scalar_elimination;
mod dispatch_and_composition;
mod proof_check_elision;
mod proof_check_operational;
mod structural_rewrites;
mod value_numbering;
