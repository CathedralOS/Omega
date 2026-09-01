//! Optimizer module role: executable entrance. Exact arithmetic-family target replay routes.

pub(crate) mod exact_add;
pub(crate) mod exact_divide;
pub(crate) mod exact_multiply;
pub(crate) mod exact_remainder;
pub(crate) mod exact_subtract;
mod reconstruction;
mod replay;
pub(crate) mod saturating_add;
pub(crate) mod saturating_divide;
pub(crate) mod saturating_multiply;
pub(crate) mod saturating_remainder;
pub(crate) mod saturating_subtract;
pub(crate) mod wrapping_add;
pub(crate) mod wrapping_divide;
pub(crate) mod wrapping_multiply;
pub(crate) mod wrapping_remainder;
pub(crate) mod wrapping_subtract;

// Named joins keep target leaves separate from shared ABI replay.
pub(super) use reconstruction::{
    reconstruct_exact_add, reconstruct_exact_divide, reconstruct_exact_multiply,
    reconstruct_exact_remainder, reconstruct_exact_subtract, reconstruct_saturating_add,
    reconstruct_saturating_divide, reconstruct_saturating_multiply,
    reconstruct_saturating_remainder, reconstruct_saturating_subtract, reconstruct_wrapping_add,
    reconstruct_wrapping_divide, reconstruct_wrapping_multiply, reconstruct_wrapping_remainder,
    reconstruct_wrapping_subtract,
};
