//! Obligation-free total scalar-identity validation.
//!
//! `classification` reconstructs wrapping neutral arithmetic, zero-count
//! shifts, multiply-zero annihilation, and saturating neutral arithmetic.
//! `evidence` authenticates the independently typed literal, `validation`
//! joins exact rule custody, and `application` realizes the independently
//! admitted rewrite.

use super::super::*;

mod application;
mod classification;
mod evidence;
mod validation;

pub use validation::validate_total_scalar_identity_candidate;

#[cfg(test)]
mod tests;
