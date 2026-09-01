//! Optimizer module role: stage group. Parameterless integer-conversion translation families.

use super::*;

#[path = "translation_validation_integer_exact_cast_immediate_operand/mod.rs"]
mod exact_cast;
#[path = "translation_validation_integer_widen_immediate/mod.rs"]
mod widen;
