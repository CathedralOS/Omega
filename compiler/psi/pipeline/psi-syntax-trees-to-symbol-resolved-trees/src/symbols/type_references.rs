mod assignment;
mod queries;

pub(super) use assignment::{
    assign_proposition_family_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
    assign_type_reference_symbol_with_locals_and_constraints,
    assign_type_reference_symbol_with_locals_and_self_type,
    assign_type_reference_symbol_with_locals_and_self_type_and_constraints,
    assign_type_reference_symbols,
};
pub(super) use queries::call_target_for_type_reference;
