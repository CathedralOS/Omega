//! Optimizer module role: stage group. values in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod structural;
pub use structural::{
    TargetReferenceResult, TargetStructuralArgument, TargetStructuralArgumentSource,
    TargetStructuralParameter,
};
mod byte_view;
pub use byte_view::TargetByteView;
mod element_view;
pub use element_view::*;
mod scalar;
pub use scalar::{TargetScalarExpression, TargetScalarImmediate};
mod boolean;
pub use boolean::TargetBooleanExpression;
mod integer;
pub use integer::TargetIntegerExpression;
mod block_value;
pub use block_value::TargetScalarBlockValue;
