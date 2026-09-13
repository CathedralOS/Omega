//! Optimizer module role: stage group.
//! Independent root, function-roster, signature, and occurrence settlement custody.
//! Executable graph bodies require the downstream common graph replay.
mod model;
mod structural_call_arguments;
mod structural_shapes;
mod structural_signatures;
mod whole_plan;
pub use model::*;
pub use whole_plan::{
    validate_abstract_to_target_translation,
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements,
};
