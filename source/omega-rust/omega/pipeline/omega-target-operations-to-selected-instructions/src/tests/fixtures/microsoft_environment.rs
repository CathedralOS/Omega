//! Validated Microsoft x86-64 physical model, constraint catalog, and selection keys.

use omega_isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use omega_register_model::{
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_physical_register_model,
};
use omega_selected_instructions::{SelectedConstraintKeys, SelectedSelectionConstraints};

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
            structural_unit_call: Some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR),
            materialize_i64: X86_64_MATERIALIZE_I64,
            copy_i64: X86_64_COPY_I64,
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            return_i64: X86_64_MICROSOFT_RETURN,
            return_unit: X86_64_MICROSOFT_RETURN_UNIT,
        },
        fixed_inputs: Vec::new(),
    };
    (physical, catalog, constraints)
}
