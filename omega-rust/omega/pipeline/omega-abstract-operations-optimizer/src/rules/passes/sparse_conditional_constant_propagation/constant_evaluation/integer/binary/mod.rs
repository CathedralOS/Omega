//! Optimizer module role: stage group. Binary integer folds by exact rule identity.
//!
//! Every exact entrance owns its canonical contract and proposal join. The
//! group shares only closed operation shapes, typed evaluation, constant-fact
//! traversal, and witness construction. The SCCP pass entrance remains the
//! sole owner of exact rule enablement and order.

mod evaluation;
mod exact_integer_add_constants;
mod exact_integer_divide_constants;
mod exact_integer_multiply_constants;
mod exact_integer_remainder_constants;
mod exact_integer_shift_left_constants;
mod exact_integer_shift_right_constants;
mod exact_integer_subtract_constants;
mod integer_bitwise_and_constants;
mod integer_bitwise_or_constants;
mod integer_bitwise_xor_constants;
mod model;
mod proposal;
mod saturating_integer_add_constants;
mod saturating_integer_divide_constants;
mod saturating_integer_multiply_constants;
mod saturating_integer_remainder_constants;
mod saturating_integer_subtract_constants;
mod shapes;
mod witness;
mod wrapping_integer_add_constants;
mod wrapping_integer_divide_constants;
mod wrapping_integer_multiply_constants;
mod wrapping_integer_remainder_constants;
mod wrapping_integer_shift_left_constants;
mod wrapping_integer_shift_right_constants;
mod wrapping_integer_subtract_constants;

pub use exact_integer_add_constants::ExactIntegerAddConstantsRule;
pub use exact_integer_divide_constants::ExactIntegerDivideConstantsRule;
pub use exact_integer_multiply_constants::ExactIntegerMultiplyConstantsRule;
pub use exact_integer_remainder_constants::ExactIntegerRemainderConstantsRule;
pub use exact_integer_shift_left_constants::ExactIntegerShiftLeftConstantsRule;
pub use exact_integer_shift_right_constants::ExactIntegerShiftRightConstantsRule;
pub use exact_integer_subtract_constants::ExactIntegerSubtractConstantsRule;
pub use integer_bitwise_and_constants::IntegerBitwiseAndConstantsRule;
pub use integer_bitwise_or_constants::IntegerBitwiseOrConstantsRule;
pub use integer_bitwise_xor_constants::IntegerBitwiseXorConstantsRule;
pub use saturating_integer_add_constants::SaturatingIntegerAddConstantsRule;
pub use saturating_integer_divide_constants::SaturatingIntegerDivideConstantsRule;
pub use saturating_integer_multiply_constants::SaturatingIntegerMultiplyConstantsRule;
pub use saturating_integer_remainder_constants::SaturatingIntegerRemainderConstantsRule;
pub use saturating_integer_subtract_constants::SaturatingIntegerSubtractConstantsRule;
pub use wrapping_integer_add_constants::WrappingIntegerAddConstantsRule;
pub use wrapping_integer_divide_constants::WrappingIntegerDivideConstantsRule;
pub use wrapping_integer_multiply_constants::WrappingIntegerMultiplyConstantsRule;
pub use wrapping_integer_remainder_constants::WrappingIntegerRemainderConstantsRule;
pub use wrapping_integer_shift_left_constants::WrappingIntegerShiftLeftConstantsRule;
pub use wrapping_integer_shift_right_constants::WrappingIntegerShiftRightConstantsRule;
pub use wrapping_integer_subtract_constants::WrappingIntegerSubtractConstantsRule;

use omega_optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};

pub(super) fn contract(
    rule_name: &[u8],
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    super::super::constant_evaluation_contract(rule_name, safety)
}
