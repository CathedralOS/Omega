//! Register identities, ABI constraints and physical frame plans for aarch64.

mod constraint_keys;
pub mod frame_protocol;
mod preservation_storage;
pub mod register_model;
pub use constraint_keys::*;
pub use frame_protocol::{
    Aarch64FrameProtocolError, Aarch64FrameProtocolPlan, Aarch64FrameSlot, Aarch64StackProbe,
    plan_aapcs64_frame_protocol,
};
pub use preservation_storage::*;
pub use register_model::*;
pub mod float_sizes;
