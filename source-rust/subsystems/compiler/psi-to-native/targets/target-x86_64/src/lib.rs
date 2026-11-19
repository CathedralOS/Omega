//! Register identities, ABI constraints and physical frame plans for x86_64.

mod constraint_keys;
pub mod frame_protocol;
pub mod import_thunk;
mod preservation_storage;
pub mod register_model;
pub use constraint_keys::*;
pub use frame_protocol::{
    X86_64FrameProtocolError, X86_64FrameProtocolPlan, X86_64FrameSlot, X86_64StackProbe,
    plan_system_v_amd64_frame_protocol,
};
pub use preservation_storage::*;
pub use register_model::*;
pub mod hosted_kernel;
pub use hosted_kernel::hosted_syscall_target;
pub mod callback_geometry;
pub mod inspection;
pub mod instruction_bytes;
mod model_specific_state;
pub mod write_line;
pub use model_specific_state::x86_64_model_specific_register_state;
pub const PRIVILEGED_STOP_BYTES: [u8; 4] = [0xfa, 0xf4, 0xeb, 0xfd];
mod generated_entry;
pub use generated_entry::decode_x86_64_generated_entry;
pub const ORDINARY_DIRECT_CALL_BYTES: [u8; 5] = [0xe8, 0, 0, 0, 0];
pub mod float_sizes;
pub mod form_effects;
pub mod machine_effects;
pub fn encoder_resolved(
    minimum_bytes: u16,
    maximum_bytes: Option<u16>,
) -> selected_instructions::MachineSizeKnowledge {
    selected_instructions::MachineSizeKnowledge::EncoderResolved {
        minimum_bytes,
        maximum_bytes,
    }
}
pub use machine_effects::{
    X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
    x86_64_machine_effect_catalog, x86_64_normalized_foreign_call_declaration,
};
