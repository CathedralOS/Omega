//! Obligation-free wrapping neutral-arithmetic identity validation.
//!
//! `classification` reconstructs the five total laws, `evidence` authenticates
//! the neutral literal, `validation` joins candidate custody, and `application`
//! realizes the independently admitted rewrite.

use super::super::*;

mod application;
mod classification;
mod evidence;
mod validation;

pub use validation::validate_total_scalar_identity_candidate;

#[cfg(test)]
mod tests;
