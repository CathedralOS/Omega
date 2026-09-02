//! Optimizer module role: stage group. Assignment behavior is classified by
//! the operation family whose physical representation and custody it preserves.

mod boolean_cleanup;
mod dynamic_scalar_dispatch;
mod expression_homes;
mod installed_provider_scalar;
mod native_callback;
mod ranked_countdown;
mod structural_calls;
mod structural_scalar_unit;
