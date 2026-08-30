//! Optimizer module role: stage group. Obligation-free total scalar-identity validation.
//!
//! `classification` reconstructs wrapping, saturating, and distinct bitwise
//! neutral/absorbing families. `evidence` authenticates the independently
//! typed law literal, `validation` joins exact rule custody, and `application`
//! realizes the independently admitted rewrite.

use super::super::*;

mod application;
mod classification;
mod evidence;
mod validation;

pub use validation::validate_total_scalar_identity_candidate;

#[cfg(test)]
mod tests;
