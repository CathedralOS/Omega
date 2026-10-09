//! Immutable frame geometry and the requirements from which it was derived.
//!
//! These records grant no validation or emission authority. The backend owns
//! construction, independent replay, and the subsequent frame protocol.

mod callee_save;
mod layout;
pub use callee_save::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity,
    NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStoragePolicy,
};
pub use layout::{
    CalleeSaveFrameSlot, FrameUnwindPlan, FrameUnwindRestore, FunctionTargetFrameLayout,
    LocalStorageFrameSlot, OutgoingAbiFrameArea, ReturnAddressFrameCustody, StackProbePlan,
    TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
};
