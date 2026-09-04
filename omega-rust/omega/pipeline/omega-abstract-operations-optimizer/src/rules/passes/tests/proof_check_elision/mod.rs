//! Optimizer module role: stage group. Proof-check-elision tests, grouped by the explicit identity being removed.

use super::*;

mod catalog;
mod contract_custody;
mod divide_by_one;
mod multiply_by_zero;
mod negative_one_shift_right;
mod remainder_by_one;
mod scalar_identities;
mod self_divide;
mod self_remainder;
mod self_subtract;
mod signed_remainder_by_negative_one;
mod zero_dividend;
mod zero_value_shift;
