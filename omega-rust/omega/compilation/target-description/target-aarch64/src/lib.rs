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
pub mod callback_geometry;
pub mod float_sizes;
pub mod form_effects;
pub mod machine_effects;
pub mod saturating_forms;
pub use machine_effects::{
    Aarch64MachineEffectCatalogValidationError, aarch64_machine_effect_catalog,
    aarch64_normalized_foreign_call_declaration, validate_aarch64_machine_effect_catalog,
};
