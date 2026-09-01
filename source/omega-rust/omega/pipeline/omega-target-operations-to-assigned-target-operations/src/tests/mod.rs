//! Optimizer module role: stage group. Assignment behavior is classified by
//! the operation family whose physical representation and custody it preserves.

mod boolean_cleanup;
mod expression_homes;
mod native_callback;
mod ranked_countdown;
mod structural_calls;
mod structural_scalar_unit;
