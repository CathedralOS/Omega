//! Optimizer module role: stage group. Exact total scalar identity rule-family coverage.
//!
//! Each leaf owns one manifest rule row, so its contract, semantic partition,
//! rejection boundary, and replay evidence can be found from this entrance.

use super::*;

mod bitwise_absorbing;
mod bitwise_neutral;
mod catalog;
mod saturating_multiply_zero;
mod saturating_neutral;
mod wrapping_multiply_zero;
mod wrapping_neutral;
mod wrapping_shift_zero_count;
