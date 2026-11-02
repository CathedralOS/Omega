//! Optimizer module role: executable entrance. Allocation-recovery rule entrance.
//!
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] is the only enable/order declaration
//! for this phase. This entrance admits its current single-rule execution
//! contract; named rule folders own proposal and independent replay.

pub(crate) mod fixed_view_copy;
pub(crate) mod pressure_rematerialization;

pub(crate) use fixed_view_copy::materialize_fixed_view_copies;
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, ValidatedFixedViewCopies, fixed_view_copy_identity,
};
pub(crate) use fixed_view_copy::{FixedViewCopyError, FixedViewCopyValidationReceipt};
pub(crate) use pressure_rematerialization::{
    FunctionPressureRematerialization, PressureRematerializationAction,
    PressureRematerializationPlan, PressureRematerializationRewrite,
    PressureRematerializationValidationReceipt, pressure_rematerialization_identity,
};
pub use pressure_rematerialization::{
    PressureRematerializationError, PressureRematerializationPolicy,
    ValidatedPressureRematerialization, rematerialize_selected_active_resident,
};
