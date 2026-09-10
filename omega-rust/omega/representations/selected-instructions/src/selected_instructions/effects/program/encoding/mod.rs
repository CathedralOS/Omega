//! Optimizer module role: stage group. Versioned persistence for pre-allocation machine effects.
//!
//! Only the current wire version is admitted. `v6` retains the original payload
//! taxonomy; its framing rejects older vocabularies before decoding instructions.

mod cursor;
mod error;
mod v6;

pub use cursor::Cursor;
pub use error::PreAllocationMachineEffectDecodeError;
pub use v6::{
    decode_alternative, decode_alternative_legacy, decode_alternative_without_jump,
    decode_alternative_without_scalar_call, decode_effect_link, decode_local_storage_slot,
    decode_ownership, decode_provenance, decode_target,
    decode_terminal_pre_allocation_machine_effect_plan, decode_units,
    encode_terminal_pre_allocation_machine_effect_plan,
};
