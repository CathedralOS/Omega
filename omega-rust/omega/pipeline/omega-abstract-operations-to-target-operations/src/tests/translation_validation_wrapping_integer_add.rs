//! Optimizer module role: stage group. Wrapping integer-add translation families by operand source.

use super::*;

#[path = "translation_validation_wrapping_integer_add_immediate/mod.rs"]
mod immediate;
#[path = "translation_validation_wrapping_integer_add_parameters/mod.rs"]
mod parameters;
