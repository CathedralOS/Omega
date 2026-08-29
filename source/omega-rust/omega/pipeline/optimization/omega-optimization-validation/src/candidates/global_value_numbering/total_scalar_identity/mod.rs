//! Obligation-free wrapping neutral-arithmetic identity validation.
//!
//! `classification` reconstructs the five neutral-arithmetic, two
//! zero-count-shift, and two multiply-zero laws. `evidence` authenticates the
//! independently typed literal, `validation` joins exact rule custody, and
//! `application` realizes the independently admitted rewrite.

use super::super::*;

mod application;
mod classification;
mod evidence;
mod validation;

pub use validation::validate_total_scalar_identity_candidate;

#[cfg(test)]
mod tests;
