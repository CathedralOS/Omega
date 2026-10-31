//! The x86-64 register model.
//!
//! `physical_model.rs` lays out the physical registers, `operation_keys.rs`
//! and `call_keys.rs` name the operation and call constraint keys, and
//! `constraint_catalog.rs` builds and validates the constraint catalog; the
//! sibling files carry the call families and packed memory forms.

/// One operand row whose home is exactly `view`.
pub(super) fn fixed_operand(
    operand: u16,
    access: RegisterOperandAccess,
    view: &RegisterView,
) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        operand,
        access,
        class: view.class,
        fixed_view: Some(view.id),
        tied_to: None,
        early_clobber: false,
    }
}

/// One operand row that takes any register of `class`.
pub(super) fn allocatable_operand(
    operand: u16,
    access: RegisterOperandAccess,
    class: RegisterClassId,
) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        operand,
        access,
        class,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    }
}

/// The contiguous operand lane the byte-loop forms share: the first `inputs`
/// operands are read, the rest are written and early-clobbered, because the
/// loop writes its cursors and verdict before the preserved inputs are dead.
pub(super) fn operand_lane(count: u16, inputs: u16) -> Vec<RegisterOperandConstraint> {
    (0..count)
        .map(|operand| {
            let written = operand >= inputs;
            let row = allocatable_operand(
                operand,
                if written {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                },
                physical_model::GPR64,
            );
            if written { early_clobber(row) } else { row }
        })
        .collect()
}

/// Mark a written row's home as consumed before the remaining inputs are read.
pub(super) fn early_clobber(operand: RegisterOperandConstraint) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        early_clobber: true,
        ..operand
    }
}

/// The canonical x86-64 view named `name`.
///
/// The model is a fixed table, so a miss is a crate bug rather than a caller
/// error: every effect owner that names one register goes through here instead
/// of restating the lookup and its message.
pub(crate) fn view_of(name: &str) -> &'static RegisterView {
    physical_model::x86_64_physical_register_model_ref()
        .view_named(name)
        .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
}

/// Units of the canonical view named `name`.
pub(crate) fn units(name: &str) -> Vec<RegisterUnitId> {
    view_of(name).units.clone()
}

/// Sorted, deduplicated units of every canonical view named in `names`.
pub(crate) fn units_of(names: &[&str]) -> Vec<RegisterUnitId> {
    sorted_units(
        names
            .iter()
            .flat_map(|name| view_of(name).units.iter().copied()),
    )
}

use register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterOperandConstraint, RegisterUnitId,
    RegisterView, sorted_units,
};

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
mod system_calls;
pub use system_calls::x86_64_linux_system_call_keys;
#[cfg(test)]
mod tests;

pub(crate) use call_keys::X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64;
pub use call_keys::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN,
    X86_64_SYSTEM_V_RETURN_UNIT, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_normalized_foreign_call_keys,
    x86_64_microsoft_register_call_keys, x86_64_microsoft_register_unit_call_keys,
    x86_64_preservation_convention_for_target, x86_64_system_v_aggregate_call_keys,
    x86_64_system_v_aggregate_return_keys, x86_64_system_v_normalized_foreign_call_keys,
    x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
};
pub(crate) use constraint_catalog::x86_64_register_constraint_catalog_for;
pub use constraint_catalog::{
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_register_constraint_catalog,
};
pub use float_scalar_calls::{x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys};
pub use indirect_results::x86_64_indirect_aggregate_call_keys;
pub use mixed_aggregate_calls::{
    x86_64_microsoft_mixed_aggregate_call_keys, x86_64_system_v_mixed_aggregate_call_keys,
};
pub use mixed_calls::{
    x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_unit_call_keys,
};
pub use operation_keys::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_ADDRESS_OFFSET, X86_64_ATOMIC_BITWISE_UPDATE,
    X86_64_ATOMIC_COMPARE_EXCHANGE, X86_64_ATOMIC_FENCE, X86_64_ATOMIC_LOAD, X86_64_ATOMIC_STORE,
    X86_64_ATOMIC_UPDATE, X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64,
    X86_64_BYTE_LITERAL_IMAGE_ADDRESS, X86_64_COMPARE_BYTE_SEQUENCES, X86_64_COMPARE_I64,
    X86_64_COMPARE_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_CONTROL_REGISTER_READ, X86_64_CONTROL_REGISTER_WRITE, X86_64_COPY_BYTES,
    X86_64_COPY_I64, X86_64_CRASH, X86_64_DIVIDE_I64, X86_64_DIVIDE_U64, X86_64_FLAGS_RESTORE,
    X86_64_FLAGS_SNAPSHOT, X86_64_FLOAT_RESULT_SATURATING, X86_64_FLOAT_RESULT_TRAPPING,
    X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS, X86_64_FRAME_ADDRESS,
    X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE, X86_64_HOSTED_WRITE_BYTE_I32,
    X86_64_IEEE_FLOAT_ARITHMETIC, X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
    X86_64_IEEE_FLOAT_DIRECTED_BINARY, X86_64_IEEE_FLOAT_DIRECTED_UNARY, X86_64_IEEE_FLOAT_TERNARY,
    X86_64_IEEE_FLOAT_UNARY, X86_64_INTERRUPT_CONTROL, X86_64_JUMP, X86_64_LOAD_ENCODED_SCALAR,
    X86_64_LOAD8, X86_64_LOAD8_INDEXED, X86_64_LOAD16, X86_64_LOAD32, X86_64_LOAD64,
    X86_64_MATERIALIZE_BOOLEAN, X86_64_MATERIALIZE_I64, X86_64_MEMORY_FENCE,
    X86_64_MODEL_SPECIFIC_READ, X86_64_MODEL_SPECIFIC_WRITE, X86_64_MULTIPLY_I64,
    X86_64_PLACED_READ, X86_64_PLACED_WRITE, X86_64_PORT_REGISTER_READ, X86_64_PORT_REGISTER_WRITE,
    X86_64_PORT_WRITE, X86_64_PROCESSOR_HALT, X86_64_REMAINDER_I64, X86_64_REMAINDER_U64,
    X86_64_RESTORE_FLOATING_CONTROL, X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64,
    X86_64_SATURATING_DIVIDE_SIGNED, X86_64_SATURATING_MULTIPLY_CLAMPED,
    X86_64_SATURATING_MULTIPLY_U64, X86_64_SATURATING_SUBTRACT_CLAMPED,
    X86_64_SATURATING_SUBTRACT_UNSIGNED, X86_64_SAVE_FLOATING_CONTROL, X86_64_SCAN_BYTE_SEQUENCE,
    X86_64_SHIFT_I64, X86_64_STORE, X86_64_STORE_ENCODED_SCALAR, X86_64_STORE64,
    X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_TRAPPING_BINARY,
    X86_64_TRAPPING_CONVERT, X86_64_TRAPPING_FIXED_PAIR, X86_64_TRAPPING_SHIFT, X86_64_ZERO_BYTES,
};
pub(crate) use operation_keys::{
    X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_LINUX_SYSTEM_CALL, x86_64_required_register_constraints,
};
pub use packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
pub(crate) use physical_model::{
    canonical_x86_64_physical_register_model_identity, x86_64_physical_register_model_ref,
};
pub use physical_model::{
    validated_x86_64_physical_register_model, x86_64_fixed_register_view,
    x86_64_physical_register_model,
};
