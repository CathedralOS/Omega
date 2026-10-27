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
    decode_alternative, decode_local_storage_slot, decode_ownership, decode_provenance,
    decode_target, decode_units, saturating_kind,
};
pub(crate) use v6::{
    decode_terminal_pre_allocation_machine_effect_plan,
    encode_terminal_pre_allocation_machine_effect_plan,
};
