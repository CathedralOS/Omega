//! Optimizer module role: stage group. operations in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod unit;
pub use unit::{
    NativeCallOrigin, TargetFloatResultPolicy, TargetStructuralRuntimeIndex, TargetUnitOperation,
};

mod atomic;
pub use atomic::atomic_storage_integer_type;
