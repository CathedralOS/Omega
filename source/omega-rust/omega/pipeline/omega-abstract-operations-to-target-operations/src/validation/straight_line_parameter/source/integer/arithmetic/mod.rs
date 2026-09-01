//! Optimizer module role: executable entrance. Exact arithmetic-family source replay routes.

pub(in crate::validation::straight_line_parameter) mod exact_add;
pub(in crate::validation::straight_line_parameter) mod exact_divide;
pub(in crate::validation::straight_line_parameter) mod exact_multiply;
pub(in crate::validation::straight_line_parameter) mod exact_remainder;
pub(in crate::validation::straight_line_parameter) mod exact_subtract;
pub(in crate::validation::straight_line_parameter) mod reconstruction;
pub(in crate::validation::straight_line_parameter) mod saturating_add;
pub(in crate::validation::straight_line_parameter) mod saturating_multiply;
pub(in crate::validation::straight_line_parameter) mod saturating_subtract;
pub(in crate::validation::straight_line_parameter) mod wrapping_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_divide;
pub(in crate::validation::straight_line_parameter) mod wrapping_multiply;
pub(in crate::validation::straight_line_parameter) mod wrapping_subtract;

// Named joins keep classifiers separate from reconstruction details.
pub(in crate::validation::straight_line_parameter) use reconstruction::{
    reconstruct_exact_add, reconstruct_exact_divide, reconstruct_exact_multiply,
    reconstruct_exact_remainder, reconstruct_exact_subtract, reconstruct_saturating_add,
    reconstruct_saturating_multiply, reconstruct_saturating_subtract, reconstruct_wrapping_add,
    reconstruct_wrapping_divide, reconstruct_wrapping_multiply, reconstruct_wrapping_subtract,
};
