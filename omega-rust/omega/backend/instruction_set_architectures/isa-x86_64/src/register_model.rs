//! The x86-64 register model.
//!
//! `physical_model.rs` lays out the physical registers, `operation_keys.rs`
//! and `call_keys.rs` name the operation and call constraint keys, and
//! `constraint_catalog.rs` builds and validates the constraint catalog; the
//! sibling files carry the call families and packed memory forms.

mod call_keys;
mod constraint_catalog;
mod float_scalar_calls;
#[cfg(test)]
mod float_transport_tests;
mod indirect_results;
mod mixed_aggregate_calls;
mod mixed_calls;
mod operation_keys;
mod packed_memory;
mod physical_model;
#[cfg(test)]
mod tests;

pub use call_keys::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_normalized_foreign_call_keys,
    x86_64_microsoft_register_call_keys, x86_64_microsoft_register_unit_call_keys,
    x86_64_preservation_convention_for_target, x86_64_system_v_aggregate_call_keys,
    x86_64_system_v_aggregate_return_keys, x86_64_system_v_normalized_foreign_call_keys,
    x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
};
pub use constraint_catalog::{
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_register_constraint_catalog,
};
pub use float_scalar_calls::*;
pub use indirect_results::*;
pub use mixed_aggregate_calls::*;
pub use mixed_calls::*;
pub use operation_keys::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_ADDRESS_OFFSET, X86_64_BITS_TO_FLOAT32,
    X86_64_BITS_TO_FLOAT64, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_BYTES, X86_64_COPY_I64,
    X86_64_DIVIDE_U64, X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS, X86_64_FRAME_ADDRESS,
    X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE, X86_64_HOSTED_WRITE_BYTE_I32,
    X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_JUMP, X86_64_LINUX_SYSTEM_CALL, X86_64_LOAD8,
    X86_64_LOAD8_INDEXED, X86_64_LOAD16, X86_64_LOAD32, X86_64_LOAD64, X86_64_MATERIALIZE_BOOLEAN,
    X86_64_MATERIALIZE_I64, X86_64_MULTIPLY_I64, X86_64_REMAINDER_I64,
    X86_64_REQUIRED_REGISTER_CONSTRAINTS, X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64,
    X86_64_SATURATING_DIVIDE_SIGNED, X86_64_SATURATING_SUBTRACT_CLAMPED,
    X86_64_SATURATING_SUBTRACT_UNSIGNED, X86_64_STORE, X86_64_STORE64, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE,
};
pub use packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
pub use physical_model::{x86_64_fixed_register_view, x86_64_physical_register_model};
