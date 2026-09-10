//! Validated Microsoft x86-64 physical model, constraint catalog, and selection keys.

use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_ZERO,
    X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use register_model::{
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_physical_register_model,
};
use selected_instructions::{SelectedConstraintKeys, SelectedSelectionConstraints};

pub(in crate::tests) fn microsoft_selection_environment() -> (
    ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
    SelectedSelectionConstraints,
) {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: SelectedConstraintKeys {
            call_aggregate: isa_x86_64::x86_64_microsoft_aggregate_call_keys(),
            return_aggregate: isa_x86_64::x86_64_microsoft_aggregate_return_keys(),
            hosted_exit_process_i32: None,
            hosted_write_byte_i32: None,
            hosted_read_byte: None,
            call_unit: isa_x86_64::x86_64_microsoft_register_unit_call_keys(),
            call_unit_mixed: Vec::new(),
            load64: Some(isa_x86_64::X86_64_LOAD64),
            load32: Some(isa_x86_64::X86_64_LOAD32),
            load8: Some(isa_x86_64::X86_64_LOAD8),
            load16: Some(isa_x86_64::X86_64_LOAD16),
            load8_indexed: None,
            store64: Some(isa_x86_64::X86_64_STORE64),
            frame_address: Some(isa_x86_64::X86_64_FRAME_ADDRESS),
            store: Some(isa_x86_64::X86_64_STORE),
            address_offset: Some(isa_x86_64::X86_64_ADDRESS_OFFSET),
            call_i64: Vec::new(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: isa_x86_64::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: None,
            float64_to_bits: None,
            bits_to_float32: None,
            bits_to_float64: None,
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: X86_64_COMPARE_I64,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: isa_x86_64::X86_64_JUMP,
            return_i64: X86_64_MICROSOFT_RETURN,
            return_unit: X86_64_MICROSOFT_RETURN_UNIT,
        },
        projected_structural_call: Some(X86_64_MICROSOFT_CALL),
        fixed_inputs: Vec::new(),
    };
    (physical, catalog, constraints)
}
