//! Optimizer module role: stage group. Versioned persistence for pre-allocation machine effects.
//!
//! The root maps supported wire versions and owns only cursor/error mechanics
//! shared across versions. `v6` owns the byte-stable V6 payload taxonomy and
//! the append-only V7 vocabulary; framing distinguishes their identities.

mod cursor;
mod error;
mod v6;

pub(crate) use cursor::Cursor;
pub use error::PreAllocationMachineEffectDecodeError;
pub(crate) use v6::{
    decode_alternative, decode_alternative_legacy, decode_alternative_without_scalar_call,
    decode_effect_link, decode_ownership, decode_provenance, decode_structural_call, decode_target,
    decode_terminal_pre_allocation_machine_effect_plan, decode_units,
    encode_terminal_pre_allocation_machine_effect_plan,
};
