//! The AArch64 register model.
//!
//! `physical_model.rs` lays out the physical registers, `operation_keys.rs`
//! and `call_keys.rs` name the operation and call constraint keys, and
//! `constraint_catalog.rs` builds and validates the constraint catalog; the
//! sibling files carry the call families.

mod call_keys;
mod constraint_catalog;
mod float_scalar_calls;
#[cfg(test)]
mod float_transport_tests;
mod indirect_results;
mod mixed_calls;
mod operation_keys;
mod physical_model;
#[cfg(test)]
mod tests;

pub use call_keys::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, aarch64_aapcs64_register_call_keys,
    aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_register_call_keys,
    aarch64_darwin_register_unit_call_keys, aarch64_preservation_convention_for_target,
    aarch64_register_aggregate_call_keys, aarch64_register_aggregate_return_keys,
};
pub use constraint_catalog::{
    Aarch64RegisterConstraintCatalogValidationError, aarch64_register_constraint_catalog,
    validate_aarch64_register_constraint_catalog,
};
pub use float_scalar_calls::*;
pub use indirect_results::*;
pub use mixed_calls::*;
pub use operation_keys::{
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_ADDRESS_OFFSET, AARCH64_BITS_TO_FLOAT32,
    AARCH64_BITS_TO_FLOAT64, AARCH64_COMPARE_I64, AARCH64_COMPARE_I64_IMMEDIATE,
    AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_BYTES, AARCH64_COPY_I64,
    AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_DIVIDE_U64, AARCH64_FLOAT32_TO_BITS,
    AARCH64_FLOAT64_TO_BITS, AARCH64_FRAME_ADDRESS, AARCH64_HOSTED_EXIT_PROCESS_I32,
    AARCH64_HOSTED_READ_BYTE, AARCH64_HOSTED_WRITE_BYTE_I32, AARCH64_INLINE_ASSEMBLY_DEFAULT,
    AARCH64_JUMP, AARCH64_LINUX_SYSTEM_CALL, AARCH64_LOAD_PACKED, AARCH64_LOAD8,
    AARCH64_LOAD8_INDEXED, AARCH64_LOAD16, AARCH64_LOAD32, AARCH64_LOAD64,
    AARCH64_MATERIALIZE_BOOLEAN, AARCH64_MATERIALIZE_I64, AARCH64_MULTIPLY_I64,
    AARCH64_REMAINDER_I64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SATURATING_ADD_CLAMPED,
    AARCH64_SATURATING_ADD_U64, AARCH64_SATURATING_DIVIDE_SIGNED,
    AARCH64_SATURATING_SUBTRACT_CLAMPED, AARCH64_SATURATING_SUBTRACT_UNSIGNED, AARCH64_STORE,
    AARCH64_STORE_PACKED, AARCH64_STORE64, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
};
pub use physical_model::{aarch64_fixed_register_view, aarch64_physical_register_model};
