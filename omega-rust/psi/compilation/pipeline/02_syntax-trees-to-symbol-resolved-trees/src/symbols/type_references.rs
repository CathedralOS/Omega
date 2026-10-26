mod assignment;
mod queries;
mod value_expressions;

pub(super) use value_expressions::{
    assign_declaration_type_value_expression_symbols, assign_type_value_expression_symbols,
    each_type_reference_handle, reassign_type_reference_symbols,
    reassign_type_reference_symbols_and_values,
};

pub(super) use assignment::{
    assign_generic_application_origin_symbols,
    assign_machine_declaration_identity_argument_symbols,
    assign_proposition_family_argument_symbols, assign_trait_bound_slot_argument_symbols,
    assign_trait_implementation_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
    assign_type_reference_symbol_with_locals,
    assign_type_reference_symbol_with_locals_and_constraints, assign_type_reference_symbols,
};
pub(super) use queries::call_target_for_type_reference;

pub(crate) use assignment::assign_nominal_machine_requirement_arguments;
